pub fn test_triple_quote() {
    let source = r#"x = """hello
world
"""
print(x)"#;
    match lexer::lex_source(source) {
        Ok(tokens) => {
            println!("Tokens:");
            for t in tokens.iter().take(40) {
                println!("{:?}", t);
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}
