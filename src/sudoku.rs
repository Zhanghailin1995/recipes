use std::{ffi::CString, os::raw::c_char};
// extern "C" {
//     fn solve_sudoku(puzzle: *const libc::c_char, length: libc::c_int);
// }
#[cxx::bridge]
mod ffi {
    extern "C++" { 
        include!("recipes/include/sudoku.h");

        unsafe fn solve_sudoku(puzzle: *const c_char, length: i32) -> String;
    }
}


pub fn sudoku_resolve(puzzle: &str) -> String {
    unsafe {
        let length = puzzle.len() as i32;
        let c_str = CString::new(puzzle).unwrap();
        
        let puzzle = c_str.as_ptr() as *const c_char;
        ffi::solve_sudoku(puzzle, length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sudoku_resolve() {
        let puzzle = "000000010400000000020000000000050407008000300001090000300400200050100000000806000";
        let result = sudoku_resolve(puzzle);
        println!("{}", result);
    }
}