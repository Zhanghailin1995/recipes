fn main() {
    cxx_build::bridge("src/sudoku.rs")
        .file("src/sudoku.cc")
        .flag_if_supported("-std=c++11")
        .compile("recipes");
}