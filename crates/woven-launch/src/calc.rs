//! Simple expression calculator for the launcher.
//! Handles: +, -, *, /, ^, parentheses, and common math functions.

pub fn evaluate(expr: &str) -> Option<f64> {
    let expr = expr.trim();
    if expr.is_empty() { return None; }
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_expr(&tokens, &mut pos)?;
    if pos != tokens.len() { return None; }
    Some(result)
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Op(char),
    LParen,
    RParen,
    Func(String),
}

fn tokenize(s: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' => { i += 1; }
            '+' | '-' => {
                // handle unary minus/plus
                let is_unary = tokens.is_empty()
                    || matches!(tokens.last(), Some(Token::Op(_) | Token::LParen));
                if is_unary && chars[i] == '-' {
                    // parse as negative number or unary
                    tokens.push(Token::Num(0.0));
                    tokens.push(Token::Op('-'));
                } else if is_unary && chars[i] == '+' {
                    // ignore unary plus
                } else {
                    tokens.push(Token::Op(chars[i]));
                }
                i += 1;
            }
            '*' | '/' | '^' => { tokens.push(Token::Op(chars[i])); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
                let num: f64 = chars[start..i].iter().collect::<String>().parse().ok()?;
                tokens.push(Token::Num(num));
            }
            'a'..='z' | 'A'..='Z' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() { i += 1; }
                let name: String = chars[start..i].iter().collect::<String>().to_lowercase();
                match name.as_str() {
                    "pi" => tokens.push(Token::Num(std::f64::consts::PI)),
                    "e" => tokens.push(Token::Num(std::f64::consts::E)),
                    _ => tokens.push(Token::Func(name)),
                }
            }
            _ => return None,
        }
    }
    Some(tokens)
}

// recursive descent: expr → term ((+|-) term)*
fn parse_expr(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('+') => { *pos += 1; left += parse_term(tokens, pos)?; }
            Token::Op('-') => { *pos += 1; left -= parse_term(tokens, pos)?; }
            _ => break,
        }
    }
    Some(left)
}

// term → power ((*|/) power)*
fn parse_term(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    let mut left = parse_power(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('*') => { *pos += 1; left *= parse_power(tokens, pos)?; }
            Token::Op('/') => { *pos += 1; left /= parse_power(tokens, pos)?; }
            _ => break,
        }
    }
    Some(left)
}

// power → atom (^ power)?  (right-associative)
fn parse_power(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    let base = parse_atom(tokens, pos)?;
    if *pos < tokens.len() && matches!(&tokens[*pos], Token::Op('^')) {
        *pos += 1;
        let exp = parse_power(tokens, pos)?;
        Some(base.powf(exp))
    } else {
        Some(base)
    }
}

// atom → number | '(' expr ')' | func '(' expr ')'
fn parse_atom(tokens: &[Token], pos: &mut usize) -> Option<f64> {
    if *pos >= tokens.len() { return None; }
    match &tokens[*pos] {
        Token::Num(n) => { let v = *n; *pos += 1; Some(v) }
        Token::LParen => {
            *pos += 1;
            let v = parse_expr(tokens, pos)?;
            if *pos < tokens.len() && matches!(&tokens[*pos], Token::RParen) { *pos += 1; }
            Some(v)
        }
        Token::Func(name) => {
            let name = name.clone();
            *pos += 1;
            // expect '('
            if *pos >= tokens.len() || !matches!(&tokens[*pos], Token::LParen) { return None; }
            *pos += 1;
            let arg = parse_expr(tokens, pos)?;
            if *pos < tokens.len() && matches!(&tokens[*pos], Token::RParen) { *pos += 1; }
            apply_func(&name, arg)
        }
        _ => None,
    }
}

fn apply_func(name: &str, arg: f64) -> Option<f64> {
    Some(match name {
        "sqrt" => arg.sqrt(),
        "abs" => arg.abs(),
        "sin" => arg.sin(),
        "cos" => arg.cos(),
        "tan" => arg.tan(),
        "ln" => arg.ln(),
        "log" => arg.log10(),
        "floor" => arg.floor(),
        "ceil" => arg.ceil(),
        "round" => arg.round(),
        _ => return None,
    })
}
