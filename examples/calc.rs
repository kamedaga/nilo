// Minimal expression parser and evaluator for + - * / and parentheses
// Implements shunting-yard to RPN, then evaluates.

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Op(char),
    LParen,
    RParen,
}

pub fn eval_expression(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    let rpn = to_rpn(&tokens)?;
    eval_rpn(&rpn)
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '0'..='9' | '.' => {
                buf.push(ch);
                while let Some('0'..='9') | Some('.') = chars.peek().copied() {
                    buf.push(chars.next().unwrap());
                }
                let v: f64 = buf.parse().map_err(|_| format!("invalid number: {}", buf))?;
                out.push(Tok::Num(v));
                buf.clear();
            }
            '+' | '-' | '*' | '/' => out.push(Tok::Op(ch)),
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            c if c.is_whitespace() => {}
            _ => return Err(format!("invalid token: {}", ch)),
        }
    }
    Ok(out)
}

fn precedence(op: char) -> i32 {
    match op {
        '+' | '-' => 1,
        '*' | '/' => 2,
        _ => 0,
    }
}

fn to_rpn(tokens: &[Tok]) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut ops: Vec<char> = Vec::new();
    for t in tokens {
        match t {
            Tok::Num(_) => out.push(t.clone()),
            Tok::Op(op) => {
                while let Some(&top) = ops.last() {
                    if top != '(' && precedence(top) >= precedence(*op) {
                        out.push(Tok::Op(ops.pop().unwrap()));
                    } else {
                        break;
                    }
                }
                ops.push(*op);
            }
            Tok::LParen => ops.push('('),
            Tok::RParen => {
                while let Some(top) = ops.pop() {
                    if top == '(' {
                        break;
                    } else {
                        out.push(Tok::Op(top));
                    }
                }
            }
        }
    }
    while let Some(top) = ops.pop() {
        if top == '(' {
            return Err("mismatched parentheses".into());
        }
        out.push(Tok::Op(top));
    }
    Ok(out)
}

fn eval_rpn(rpn: &[Tok]) -> Result<f64, String> {
    let mut st: Vec<f64> = Vec::new();
    for t in rpn {
        match t {
            Tok::Num(v) => st.push(*v),
            Tok::Op(op) => {
                let (b, a) = (st.pop(), st.pop());
                let (a, b) = match (a, b) { (Some(a), Some(b)) => (a, b), _ => return Err("invalid expression".into()) };
                let v = match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => {
                        if b == 0.0 { return Err("division by zero".into()) } else { a / b }
                    }
                    _ => return Err("unsupported operator".into()),
                };
                st.push(v);
            }
            _ => return Err("invalid RPN token".into()),
        }
    }
    if st.len() == 1 { Ok(st[0]) } else { Err("invalid expression".into()) }
}


// When compiled as an example binary (cargo run --example calc),
// provide a simple CLI to evaluate an expression.
#[cfg(not(test))]
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: cargo run --example calc -- \"EXPR\"");
        eprintln!("Example: cargo run --example calc -- \"(1+2)*3-4/5\"");
        return;
    }
    let expr = args.join(" ");
    match eval_expression(&expr) {
        Ok(v) => println!("{}", v),
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
}
