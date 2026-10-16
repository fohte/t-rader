use std::collections::BTreeSet;
use std::str::FromStr;

use rust_decimal::Decimal;

const MAX_FORMULA_LENGTH: usize = 1024;
const MAX_FORMULA_NESTING: usize = 64;
const MAX_FORMULA_OPERATORS: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum FormulaExpression {
    Number(Decimal),
    Reference(String),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
}

impl FormulaExpression {
    pub(super) fn references(&self) -> BTreeSet<&str> {
        let mut references = BTreeSet::new();
        self.collect_references(&mut references);
        references
    }

    pub(super) fn evaluate(&self, value_for: &impl Fn(&str) -> Option<Decimal>) -> Option<Decimal> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Reference(name) => value_for(name),
            Self::Add(left, right) => left
                .evaluate(value_for)?
                .checked_add(right.evaluate(value_for)?),
            Self::Subtract(left, right) => left
                .evaluate(value_for)?
                .checked_sub(right.evaluate(value_for)?),
            Self::Multiply(left, right) => left
                .evaluate(value_for)?
                .checked_mul(right.evaluate(value_for)?),
            Self::Divide(left, right) => left
                .evaluate(value_for)?
                .checked_div(right.evaluate(value_for)?),
        }
    }

    fn collect_references<'a>(&'a self, references: &mut BTreeSet<&'a str>) {
        match self {
            Self::Number(_) => {}
            Self::Reference(name) => {
                references.insert(name);
            }
            Self::Add(left, right)
            | Self::Subtract(left, right)
            | Self::Multiply(left, right)
            | Self::Divide(left, right) => {
                left.collect_references(references);
                right.collect_references(references);
            }
        }
    }
}

pub(super) fn parse_formula(input: &str) -> Result<FormulaExpression, String> {
    if input.len() > MAX_FORMULA_LENGTH {
        return Err(format!(
            "formula must not exceed {MAX_FORMULA_LENGTH} bytes"
        ));
    }
    let mut parser = FormulaParser::new(input);
    let expression = parser.parse_add_subtract()?;
    parser.skip_whitespace();
    if parser.is_finished() {
        Ok(expression)
    } else {
        Err(format!("unexpected token at character {}", parser.position))
    }
}

struct FormulaParser {
    characters: Vec<char>,
    position: usize,
    nesting: usize,
    operators: usize,
}

impl FormulaParser {
    fn new(input: &str) -> Self {
        Self {
            characters: input.chars().collect(),
            position: 0,
            nesting: 0,
            operators: 0,
        }
    }

    fn parse_add_subtract(&mut self) -> Result<FormulaExpression, String> {
        let mut expression = self.parse_multiply_divide()?;
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('+') => {
                    self.add_operator()?;
                    self.position += 1;
                    expression = FormulaExpression::Add(
                        Box::new(expression),
                        Box::new(self.parse_multiply_divide()?),
                    );
                }
                Some('-') => {
                    self.add_operator()?;
                    self.position += 1;
                    expression = FormulaExpression::Subtract(
                        Box::new(expression),
                        Box::new(self.parse_multiply_divide()?),
                    );
                }
                _ => return Ok(expression),
            }
        }
    }

    fn parse_multiply_divide(&mut self) -> Result<FormulaExpression, String> {
        let mut expression = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('*') => {
                    self.add_operator()?;
                    self.position += 1;
                    expression = FormulaExpression::Multiply(
                        Box::new(expression),
                        Box::new(self.parse_unary()?),
                    );
                }
                Some('/') => {
                    self.add_operator()?;
                    self.position += 1;
                    expression = FormulaExpression::Divide(
                        Box::new(expression),
                        Box::new(self.parse_unary()?),
                    );
                }
                _ => return Ok(expression),
            }
        }
    }

    fn parse_unary(&mut self) -> Result<FormulaExpression, String> {
        self.skip_whitespace();
        if self.peek() == Some('-') {
            self.add_operator()?;
            self.position += 1;
            return Ok(FormulaExpression::Subtract(
                Box::new(FormulaExpression::Number(Decimal::ZERO)),
                Box::new(self.parse_unary()?),
            ));
        }
        if self.peek() == Some('+') {
            self.add_operator()?;
            self.position += 1;
            return self.parse_unary();
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<FormulaExpression, String> {
        self.skip_whitespace();
        match self.peek() {
            Some('(') => {
                self.position += 1;
                self.nesting += 1;
                if self.nesting > MAX_FORMULA_NESTING {
                    self.nesting -= 1;
                    return Err(format!(
                        "formula nesting must not exceed {MAX_FORMULA_NESTING} levels"
                    ));
                }
                let expression = self.parse_add_subtract();
                self.nesting -= 1;
                let expression = expression?;
                self.skip_whitespace();
                if self.peek() != Some(')') {
                    return Err("expected `)`".to_string());
                }
                self.position += 1;
                Ok(expression)
            }
            Some(character) if character.is_ascii_digit() || character == '.' => {
                self.parse_number().map(FormulaExpression::Number)
            }
            Some(character) if character.is_ascii_alphabetic() || character == '_' => {
                self.parse_reference().map(FormulaExpression::Reference)
            }
            Some(_) => Err(format!(
                "expected a query name, number, or `(` at character {}",
                self.position
            )),
            None => Err("expected a query name, number, or `(`".to_string()),
        }
    }

    fn parse_number(&mut self) -> Result<Decimal, String> {
        let start = self.position;
        let mut has_digit = false;
        let mut has_decimal_point = false;
        while let Some(character) = self.peek() {
            if character.is_ascii_digit() {
                has_digit = true;
                self.position += 1;
            } else if character == '.' && !has_decimal_point {
                has_decimal_point = true;
                self.position += 1;
            } else {
                break;
            }
        }
        if !has_digit {
            return Err("a number must contain at least one digit".to_string());
        }
        let value = self.characters[start..self.position]
            .iter()
            .collect::<String>();
        Decimal::from_str(&value).map_err(|_| format!("invalid number `{value}`"))
    }

    fn parse_reference(&mut self) -> Result<String, String> {
        let start = self.position;
        self.position += 1;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            self.position += 1;
        }
        Ok(self.characters[start..self.position]
            .iter()
            .collect::<String>())
    }

    fn peek(&self) -> Option<char> {
        self.characters.get(self.position).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.position += 1;
        }
    }

    fn is_finished(&self) -> bool {
        self.position >= self.characters.len()
    }

    fn add_operator(&mut self) -> Result<(), String> {
        if self.operators >= MAX_FORMULA_OPERATORS {
            return Err(format!(
                "formula may contain at most {MAX_FORMULA_OPERATORS} arithmetic operators"
            ));
        }
        self.operators += 1;
        Ok(())
    }
}
