// normalize-stderr-windows: "tests/ui/" -> "$$DIR/"

// Please don't change the function name, it's used by the lint
fn test_ty_id_resolution() {
    let _check_path_vec = vec!["hey"];
    let _check_path_string = String::from("marker");
    let _check_path_option = Option::Some("<3");
}

fn main() {}
