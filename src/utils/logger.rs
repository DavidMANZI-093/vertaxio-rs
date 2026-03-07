pub fn info(msg: &str) {
    println!("vertaxio.\x1b[32minfo\x1b[0m {}", msg);
}

pub fn debug(msg: &str) {
    println!("vertaxio.\x1b[36mdebug\x1b[0m {}", msg);
}

pub fn warn(msg: &str) {
    eprintln!("vertaxio.\x1b[33mwarn\x1b[0m {}", msg);
}

pub fn error(msg: &str) {
    eprintln!("vertaxio.\x1b[31merror\x1b[0m {}", msg);
}
