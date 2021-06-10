fn main() {
    cxx_build::bridge("src/sudoku.rs")
        .file("src/sudoku.cc")
        .flag_if_supported("-std=c++11")
        .compile("recipes");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/sudoku.cc");
    println!("cargo:rerun-if-changed=include/sudoku.h");
}
