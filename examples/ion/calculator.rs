use lutetium_ion::{Application, ButtonSize, Component, Event, SemanticColor, Theme, Variant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = Component::column("root").padding(20.0).gap(10.0);
    root.add(Component::heading("title", "ION Calculator"));
    root.add(
        Component::number_input("display", "Result")
            .placeholder("0")
            .read_only(true)
            .preferred_size(360.0, 60.0),
    );
    for (row, labels) in [
        ["7", "8", "9", "÷"],
        ["4", "5", "6", "×"],
        ["1", "2", "3", "−"],
        ["C", "0", "=", "+"],
    ]
    .iter()
    .enumerate()
    {
        let mut line = Component::row(format!("row-{row}")).gap(8.0);
        for (col, label) in labels.iter().enumerate() {
            let operator = col == 3;
            line.add(
                Component::button(format!("key-{row}-{col}"), *label)
                    .color(if operator {
                        SemanticColor::Default
                    } else {
                        SemanticColor::Primary
                    })
                    .variant(if operator {
                        Variant::Bordered
                    } else {
                        Variant::Solid
                    })
                    .size(ButtonSize::Large)
                    .flex(1.0),
            );
        }
        root.add(line);
    }
    Application::new("ION Calculator")
        .size(400, 470)
        .theme(Theme::light())
        .content(root)
        .on_event(|event, tree, _theme| {
            let Event::ButtonPressed(id) = event else {
                return;
            };
            let Some(key) = id.strip_prefix("key-") else {
                return;
            };
            let (row, col) = key.split_once('-').unwrap_or(("", ""));
            let label = match (row, col) {
                ("0", "0") => "7",
                ("0", "1") => "8",
                ("0", "2") => "9",
                ("0", "3") => "/",
                ("1", "0") => "4",
                ("1", "1") => "5",
                ("1", "2") => "6",
                ("1", "3") => "*",
                ("2", "0") => "1",
                ("2", "1") => "2",
                ("2", "2") => "3",
                ("2", "3") => "-",
                ("3", "0") => "C",
                ("3", "1") => "0",
                ("3", "2") => "=",
                ("3", "3") => "+",
                _ => return,
            };
            if label == "C" {
                if let Some(display) = tree.find_mut("display") {
                    display.set_value("");
                }
                return;
            }
            if let Some(display) = tree.find_mut("display") {
                let mut expression = display.value().unwrap_or_default().to_owned();
                if label == "=" {
                    match calculate(&expression) {
                        Ok(value) => expression = value.to_string(),
                        Err(_) => expression = "Error".into(),
                    }
                } else {
                    expression.push_str(label);
                }
                display.set_value(expression);
            }
        })
        .run()?;
    Ok(())
}

fn calculate(expression: &str) -> Result<f64, &'static str> {
    struct Parser<'a> {
        bytes: &'a [u8],
        offset: usize,
    }
    impl Parser<'_> {
        fn spaces(&mut self) {
            while self
                .bytes
                .get(self.offset)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.offset += 1;
            }
        }
        fn expression(&mut self) -> Result<f64, &'static str> {
            let mut value = self.term()?;
            loop {
                self.spaces();
                match self.bytes.get(self.offset) {
                    Some(b'+') => {
                        self.offset += 1;
                        value += self.term()?;
                    }
                    Some(b'-') => {
                        self.offset += 1;
                        value -= self.term()?;
                    }
                    _ => return Ok(value),
                }
            }
        }
        fn term(&mut self) -> Result<f64, &'static str> {
            let mut value = self.factor()?;
            loop {
                self.spaces();
                match self.bytes.get(self.offset) {
                    Some(b'*') => {
                        self.offset += 1;
                        value *= self.factor()?;
                    }
                    Some(b'/') => {
                        self.offset += 1;
                        let divisor = self.factor()?;
                        if divisor == 0.0 {
                            return Err("division by zero");
                        }
                        value /= divisor;
                    }
                    _ => return Ok(value),
                }
            }
        }
        fn factor(&mut self) -> Result<f64, &'static str> {
            self.spaces();
            if self.bytes.get(self.offset) == Some(&b'-') {
                self.offset += 1;
                return Ok(-self.factor()?);
            }
            if self.bytes.get(self.offset) == Some(&b'+') {
                self.offset += 1;
                return self.factor();
            }
            if self.bytes.get(self.offset) == Some(&b'(') {
                self.offset += 1;
                let value = self.expression()?;
                self.spaces();
                if self.bytes.get(self.offset) != Some(&b')') {
                    return Err("missing closing parenthesis");
                }
                self.offset += 1;
                return Ok(value);
            }
            let start = self.offset;
            let mut dot = false;
            while let Some(byte) = self.bytes.get(self.offset) {
                if byte.is_ascii_digit() {
                    self.offset += 1;
                } else if *byte == b'.' && !dot {
                    dot = true;
                    self.offset += 1;
                } else {
                    break;
                }
            }
            if start == self.offset {
                return Err("expected number");
            }
            std::str::from_utf8(&self.bytes[start..self.offset])
                .map_err(|_| "invalid number")?
                .parse::<f64>()
                .map_err(|_| "invalid number")
        }
    }
    let mut parser = Parser {
        bytes: expression.as_bytes(),
        offset: 0,
    };
    let value = parser.expression()?;
    parser.spaces();
    if parser.offset != parser.bytes.len() {
        Err("invalid expression")
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::calculate;
    #[test]
    fn calculator_respects_operator_precedence() {
        assert_eq!(calculate("2+3*4"), Ok(14.0));
    }
    #[test]
    fn calculator_rejects_division_by_zero() {
        assert_eq!(calculate("8/0"), Err("division by zero"));
    }
    #[test]
    fn calculator_accepts_negative_values() {
        assert_eq!(calculate("-2+5"), Ok(3.0));
    }
}
