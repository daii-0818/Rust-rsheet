use rsheet_lib::cell_value::CellValue;
use rsheet_lib::cells::{column_name_to_number, column_number_to_name};
use rsheet_lib::command_runner::{CellArgument, CommandRunner};
use rsheet_lib::replies::Reply;
use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

pub struct Spreadsheet {
    cells: HashMap<String, (String, CellValue)>,
    dependencies: HashMap<String, Vec<String>>,
}

impl Spreadsheet {
    pub fn new() -> Self {
        Spreadsheet {
            cells: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    pub fn set(&mut self, cell_name: &str, expression: &str) -> Option<Reply> {
        let runner = CommandRunner::new(expression);

        let variable_names = runner.find_variables();
        let variable_values: HashMap<String, CellArgument> = variable_names
            .iter()
            .map(|name| (name.clone(), self.resolve_variable(name)))
            .collect();

        let mut result = runner.run(&variable_values);
        for variable_name in variable_names.clone() {
            let cell_value = self
                .cells
                .get(&variable_name)
                .cloned()
                .unwrap_or(("".to_string(), CellValue::None));
            if matches!(cell_value.1, CellValue::Error(_)) {
                result = CellValue::Error("Error in dependent cells.".to_owned());
            }
        }

        self.cells.insert(
            cell_name.to_string(),
            (expression.to_string(), result.clone()),
        );

        for variable_name in variable_names {
            
            let mut vnames: Vec<String> = Vec::new();
            vnames.push(variable_name);
            for vname in self.cell_names_to_vec(vnames) {
                let dependent_cells = self.dependencies.get(&vname).cloned();
                if dependent_cells.is_some() {
                    let mut dependent_cells = dependent_cells.unwrap();
                    
                    dependent_cells.push(cell_name.to_owned());
                    self.dependencies
                        .insert(vname.to_string(), dependent_cells);
                } else {
                    
                    let dependent_cells: Vec<String> = vec![cell_name.to_owned()];


                    self.dependencies
                        .insert(vname.to_string(), dependent_cells);
                }
            }
        }

        self.update_dependent_cells(cell_name.to_string());

        
        None
    }

    pub fn get(&mut self, cell_name: &str) -> Reply {
        
        let cell_value = self
            .cells
            .get(cell_name)
            .cloned()
            .unwrap_or(("".to_string(), CellValue::None));
        if cell_value.1 == CellValue::Error("self-referential".to_owned()) {
            return Reply::Error(format!("Cell {} is self-referential", cell_name));
        } else if cell_value.1 == CellValue::Error("Error in dependent cells.".to_owned()) {
            return Reply::Error("Error in dependent cells.".to_string());
        }
        Reply::Value(cell_name.to_string(), cell_value.1)
    }

    fn update_dependent_cells(&mut self, cell_name: String) {
        if self.self_referential(cell_name.to_string()) {
            if let Some(old_value) = self.cells.get(&cell_name) {
                let old_value = old_value.clone().0;
                self.cells.insert(
                    cell_name.to_owned(),
                    (old_value, CellValue::Error("self-referential".to_owned())),
                );
                return;
            };
        }
        let dependent_cells = self.dependencies.get(&cell_name).cloned();

        if let Some(dependent_cells) = dependent_cells {
            for dependent_cell in dependent_cells {
                if let Some(expression) = self.cells.get(&dependent_cell) {
                    let expression = expression.0.clone();
                    
                    let runner = CommandRunner::new(&expression.to_string());
                    let variable_names = runner.find_variables();
                    let variable_values: HashMap<String, CellArgument> = variable_names
                        .iter()
                        .map(|name| (name.clone(), self.resolve_variable(name)))
                        .collect();

                    let result = runner.run(&variable_values);

                    self.cells
                        .insert(dependent_cell.clone(), (expression, result.clone()));

                    self.update_dependent_cells(dependent_cell);
                }
            }
        }
    } 

    fn resolve_variable(&self, variable: &str) -> CellArgument {
        if variable.contains('_') {
            self.convert_range_to_cell_argument(variable)
        } else {
            CellArgument::Value(
                self.cells
                    .get(variable)
                    .cloned()
                    .unwrap_or(("".to_string(), CellValue::None))
                    .1,
            )
        }
    }
    fn resolve_range(&self, start: &str, end: &str) -> CellArgument {
        if let (Some((start_row, start_col)), Some((end_row, end_col))) =
            (Self::parse_cell(start), Self::parse_cell(end))
        {
            let mut matrix = Vec::new();
            for row in start_row..=end_row {
                let mut row_vec = Vec::new();
                for col in start_col..=end_col {
                    
                    let cell_name = format!("{}{}", (col as u8 + b'A' - 1) as char, row);

                    row_vec.push(self.resolve_variable_to_value(&cell_name)); // Now pushing CellValue
                }
                matrix.push(row_vec);
            }

            if matrix.len() == 1 && matrix[0].len() > 1 {
                CellArgument::Vector(matrix.remove(0))
            } else {
                CellArgument::Matrix(matrix)
            }
        } else {
            CellArgument::Value(CellValue::Error("Invalid cell range".to_string()))
        }
    }

    fn convert_range_to_cell_argument(&self, range: &str) -> CellArgument {
        let bounds: Vec<&str> = range.split('_').collect();
        if bounds.len() == 2 {
            let start = bounds[0];
            let end = bounds[1];
            self.resolve_range(start, end)
        } else if let CellArgument::Value(cell_value) = self.resolve_variable(range) {
            CellArgument::Value(cell_value)
        } else {
            panic!("Single cell resolution did not return CellValue")
        }
    }

    fn resolve_variable_to_value(&self, variable: &str) -> CellValue {
        match self.resolve_variable(variable) {
            CellArgument::Value(cell_value) => cell_value,
            _ => panic!("Expected CellValue for a single cell variable."),
        }
    }

    pub fn parse_cell(cell: &str) -> Option<(usize, usize)> {
        let col = cell.chars().next()?.to_ascii_uppercase() as usize - 'A' as usize + 1;
        let row = cell[1..].parse::<usize>().ok()?;
        Some((row, col))
    }

    fn self_referential(&self, cell_name: String) -> bool {
        let dependent_cells = self.dependencies.get(&cell_name).cloned();
        let mut vec: Vec<String> = Vec::new();
        let mut num_max = 5;
        if dependent_cells.is_some() {
            let mut dependent_cells = dependent_cells.unwrap();
            while !dependent_cells.is_empty() && num_max > 0 {
                vec.push(dependent_cells[0].clone());
                
                let dependent_cells_1 = self.dependencies.get(&dependent_cells[0]).cloned();
                if dependent_cells_1.is_some() {
                    for d in dependent_cells_1.unwrap() {
                        
                        dependent_cells.push(d.clone());
                    }
                }
                dependent_cells.remove(0);
                if dependent_cells.contains(&cell_name.to_string()) {
                    return true;
                }
                num_max -= 1;
            }
        }
        
        false
    }
    
    fn cell_names_to_vec(&self, cell_names: Vec<String>) -> Vec<String> {
        let mut res: Vec<String> = Vec::new();
        for cell_name in cell_names {
            if cell_name.contains('_') {
                static RE: Lazy<Regex> = Lazy::new(|| {
                    Regex::new(r"^(?<start_c>[A-Z]+)(?<start_r>[1-9][0-9]*)_(?<end_c>[A-Z]+)(?<end_r>[1-9][0-9]*)").unwrap()
                });
                let captures = match RE.captures(&cell_name) {
                    Some(captures) => captures,
                    None => {
                        
                        continue;
                    }
                };

                let start_c = captures.name("start_c").map(|s| s.as_str()).unwrap_or("");
                let start_r = captures.name("start_r").map(|s| s.as_str()).unwrap_or("");
                let end_c = captures.name("end_c").map(|s| s.as_str()).unwrap_or("");
                let end_r = captures.name("end_r").map(|s| s.as_str()).unwrap_or("");
                let start_c = column_name_to_number(start_c);
                let end_c = column_name_to_number(end_c);
                let start_r: i32 = start_r.parse().unwrap();
                let end_r: i32 = end_r.parse().unwrap();
                
                for col in (start_c..end_c + 1).rev() {
                    for row in (start_r..end_r + 1).rev() {
                        let mut col_name = column_number_to_name(col);
                        col_name.push_str(&row.to_string());
                        
                        res.push(col_name);
                    }
                }
            } else {
                
                res.push(cell_name);
            }
        }
        res
    }
}
