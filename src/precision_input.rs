//! Transient numeric input shared by creation and editing adapters.
//!
//! Values stay in the caller's display unit. An activated value is the frozen
//! baseline for postfix arithmetic; valid ordinary drafts have a live preview.
//! No model, keyboard binding, UI focus or transaction is owned here.

use std::fmt;

const MAX_TEXT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthUnit {
    Meter,
    Centimeter,
    Millimeter,
    Foot,
    Inch,
}

impl LengthUnit {
    pub fn from_symbol(symbol: &str) -> Option<Self> {
        match symbol.trim().to_ascii_lowercase().as_str() {
            "m" => Some(Self::Meter),
            "cm" => Some(Self::Centimeter),
            "mm" => Some(Self::Millimeter),
            "ft" => Some(Self::Foot),
            "in" => Some(Self::Inch),
            _ => None,
        }
    }

    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Meter => "m",
            Self::Centimeter => "cm",
            Self::Millimeter => "mm",
            Self::Foot => "ft",
            Self::Inch => "in",
        }
    }

    pub const fn meters_per_unit(self) -> f64 {
        match self {
            Self::Meter => 1.,
            Self::Centimeter => 0.01,
            Self::Millimeter => 0.001,
            Self::Foot => 0.3048,
            Self::Inch => 0.0254,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    Length(LengthUnit),
    /// Degrees; explicit `deg` and `°` suffixes mean the same unit.
    Angle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericError {
    Incomplete,
    InvalidNumber,
    UnsupportedUnit,
    UnitMismatch,
    ScalarRequired,
    DivisionByZero,
    NonFinite,
    RejectedValue,
    InputTooLong,
}

impl NumericError {
    /// Stable text keys for the existing application localization service.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Incomplete => "Enter a complete number",
            Self::InvalidNumber => "Enter a valid number",
            Self::UnsupportedUnit => "Unsupported unit",
            Self::UnitMismatch => "Unit does not match this field",
            Self::ScalarRequired => "Use a unitless factor for multiplication or division",
            Self::DivisionByZero => "Division by zero",
            Self::NonFinite => "Value must be finite",
            Self::RejectedValue => "Value is not valid for this field",
            Self::InputTooLong => "Numeric input is too long",
        }
    }
}

impl fmt::Display for NumericError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl std::error::Error for NumericError {}

#[derive(Clone, Debug, PartialEq)]
pub struct NumericDraft {
    text: String,
    replace: bool,
    baseline: f64,
    last_valid: f64,
    error: Option<NumericError>,
    quantity: Quantity,
}

impl NumericDraft {
    /// Capture a finite displayed value without rounding away its precision.
    pub fn new(value: f64, quantity: Quantity) -> Result<Self, NumericError> {
        if !value.is_finite() {
            return Err(NumericError::NonFinite);
        }
        Ok(Self {
            text: value.to_string(),
            replace: true,
            baseline: value,
            last_valid: value,
            error: None,
            quantity,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_replacing(&self) -> bool {
        self.replace
    }

    pub fn baseline(&self) -> f64 {
        self.baseline
    }

    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    pub fn error(&self) -> Option<NumericError> {
        self.error
    }

    /// The most recent legal draft, even while current text is incomplete.
    /// This is a preview only; callers must also check `is_confirmable` before
    /// committing through either a keyboard action or a pointer click.
    pub fn preview(&self) -> f64 {
        self.last_valid
    }

    pub fn last_valid(&self) -> f64 {
        self.last_valid
    }

    pub fn is_confirmable(&self) -> bool {
        self.error.is_none()
    }

    pub fn confirm(&self) -> Result<f64, NumericError> {
        self.error.map_or(Ok(self.last_valid), Err)
    }

    pub fn select_all(&mut self) {
        self.replace = true;
    }

    /// Clear input, retaining the arithmetic baseline and last legal preview.
    pub fn clear(&mut self) {
        self.text.clear();
        self.replace = false;
        self.error = Some(NumericError::Incomplete);
    }

    pub fn backspace(&mut self, valid: impl Fn(f64) -> bool) {
        if self.replace {
            self.clear();
        } else {
            self.text.pop();
            self.refresh(&valid);
        }
    }

    /// Feed ordered text events. Each postfix operator evaluates immediately,
    /// selects its result and updates the next arithmetic baseline. This is
    /// intentionally not an infix-expression parser.
    ///
    /// The caller supplies the field's legality rule, in display units; for
    /// example, signed distance and a positive-only radius need different rules.
    pub fn input(&mut self, text: &str, valid: impl Fn(f64) -> bool) {
        if text.len() > MAX_TEXT_BYTES {
            self.error = Some(NumericError::InputTooLong);
            return;
        }
        for character in text.chars() {
            if self.replace {
                self.text.clear();
                self.replace = false;
            }
            if self.text.len() + character.len_utf8() > MAX_TEXT_BYTES {
                self.error = Some(NumericError::InputTooLong);
                return;
            }
            let sign = matches!(character, '+' | '-')
                && (self.text.trim().is_empty() || self.text.trim_end().ends_with(['e', 'E']));
            if matches!(character, '+' | '-' | '*' | '/') && !sign {
                self.postfix(character, &valid);
            } else {
                self.text.push(character);
                self.refresh(&valid);
            }
        }
    }

    fn refresh(&mut self, valid: &impl Fn(f64) -> bool) {
        match parse_value(&self.text, self.quantity, false).and_then(|value| legal(value, valid)) {
            Ok(value) => {
                self.last_valid = value;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn postfix(&mut self, operator: char, valid: &impl Fn(f64) -> bool) {
        let operand = parse_value(&self.text, self.quantity, matches!(operator, '*' | '/'));
        self.text.push(operator);
        let result = operand.and_then(|operand| {
            if operator == '/' && operand == 0. {
                return Err(NumericError::DivisionByZero);
            }
            legal(
                match operator {
                    '+' => self.baseline + operand,
                    '-' => self.baseline - operand,
                    '*' => self.baseline * operand,
                    '/' => self.baseline / operand,
                    _ => unreachable!(),
                },
                valid,
            )
        });
        match result {
            Ok(value) => {
                self.text = value.to_string();
                self.replace = true;
                self.baseline = value;
                self.last_valid = value;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}

fn legal(value: f64, valid: &impl Fn(f64) -> bool) -> Result<f64, NumericError> {
    if !value.is_finite() {
        Err(NumericError::NonFinite)
    } else if !valid(value) {
        Err(NumericError::RejectedValue)
    } else {
        Ok(value)
    }
}

fn parse_value(text: &str, quantity: Quantity, scalar: bool) -> Result<f64, NumericError> {
    let text = text.trim();
    if matches!(text, "" | "+" | "-" | "." | "+." | "-.") {
        return Err(NumericError::Incomplete);
    }
    if let Ok(value) = text.parse::<f64>() {
        return value
            .is_finite()
            .then_some(value)
            .ok_or(NumericError::NonFinite);
    }
    // Find the longest complete numeric prefix, preserving exponent syntax and
    // refusing unknown unit text rather than silently discarding its suffix.
    for (index, _) in text.char_indices().rev() {
        let Ok(number) = text[..index].trim_end().parse::<f64>() else {
            continue;
        };
        if !number.is_finite() {
            return Err(NumericError::NonFinite);
        }
        let symbol = text[index..].trim();
        if matches!(symbol, "e" | "E" | "e+" | "E+" | "e-" | "E-") {
            return Err(NumericError::Incomplete);
        }
        let length_unit = LengthUnit::from_symbol(symbol);
        let angle_unit = symbol.eq_ignore_ascii_case("deg") || symbol == "°";
        if length_unit.is_none() && !angle_unit {
            return Err(
                if symbol.starts_with(|c: char| c.is_alphabetic() || matches!(c, '°' | '\'' | '"'))
                {
                    NumericError::UnsupportedUnit
                } else {
                    NumericError::InvalidNumber
                },
            );
        }
        if scalar {
            return Err(NumericError::ScalarRequired);
        }
        let value = match (quantity, length_unit, angle_unit) {
            (Quantity::Length(display), Some(unit), false) => {
                number * unit.meters_per_unit() / display.meters_per_unit()
            }
            (Quantity::Angle, None, true) => number,
            _ => return Err(NumericError::UnitMismatch),
        };
        return value
            .is_finite()
            .then_some(value)
            .ok_or(NumericError::NonFinite);
    }
    Err(NumericError::InvalidNumber)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn millimeters(value: f64) -> NumericDraft {
        NumericDraft::new(value, Quantity::Length(LengthUnit::Millimeter)).unwrap()
    }

    #[test]
    fn precision_numeric_postfix_matches_observed_displayed_baselines() {
        for (input, expected) in [
            ("2*", 26200.),
            ("2+", 13102.),
            ("2-", 13098.),
            ("2/", 6550.),
        ] {
            let mut draft = millimeters(13100.);
            draft.input(input, f64::is_finite);
            assert_eq!(draft.confirm(), Ok(expected), "{input}");
            assert_eq!(draft.preview(), expected);
            assert_eq!(draft.baseline(), expected);
            assert!(draft.is_replacing());
        }
        let mut draft = millimeters(15757.);
        draft.input("2*3+", f64::is_finite);
        assert_eq!(draft.confirm(), Ok(31517.));
        assert_eq!(draft.text(), "31517");
    }

    #[test]
    fn precision_numeric_signs_are_field_policy_and_do_not_change_baseline() {
        let mut distance = millimeters(13100.);
        distance.input("-2", f64::is_finite);
        assert_eq!(distance.confirm(), Ok(-2.));
        assert_eq!(distance.baseline(), 13100.);
        let mut positive = millimeters(10.);
        positive.input("-2", |value| value >= 0.);
        assert_eq!(positive.confirm(), Err(NumericError::RejectedValue));
        assert_eq!(positive.preview(), 10.);
        positive.select_all();
        positive.input("2", |value| value >= 0.);
        assert_eq!(positive.confirm(), Ok(2.));
        let mut signed_factor = millimeters(10.);
        signed_factor.input("-2*", f64::is_finite);
        assert_eq!(signed_factor.confirm(), Ok(-20.));
        let mut angle = NumericDraft::new(45., Quantity::Angle).unwrap();
        angle.input("-90deg", f64::is_finite);
        assert_eq!(angle.confirm(), Ok(-90.));
    }

    #[test]
    fn precision_numeric_units_convert_to_display_units_and_ratios_are_unitless() {
        let mut draft = millimeters(13100.);
        draft.input("2m+", f64::is_finite);
        assert_eq!(draft.confirm(), Ok(15100.));
        for (unit, input, expected) in [
            (LengthUnit::Meter, "25cm", 0.25),
            (LengthUnit::Millimeter, "2ft", 609.6),
            (LengthUnit::Foot, "12in", 1.),
            (LengthUnit::Inch, "25.4mm", 1.),
        ] {
            let mut draft = NumericDraft::new(1., Quantity::Length(unit)).unwrap();
            draft.input(input, f64::is_finite);
            assert!((draft.confirm().unwrap() - expected).abs() < 1e-10);
        }
        let mut ratio = millimeters(10.);
        ratio.input("2mm*", f64::is_finite);
        assert_eq!(ratio.confirm(), Err(NumericError::ScalarRequired));
        assert_eq!(ratio.baseline(), 10.);
        assert_eq!(ratio.preview(), 2.);
        let mut angle = NumericDraft::new(45., Quantity::Angle).unwrap();
        angle.input("90°", f64::is_finite);
        assert_eq!(angle.confirm(), Ok(90.));
        angle.select_all();
        angle.input("2mm", f64::is_finite);
        assert_eq!(angle.confirm(), Err(NumericError::UnitMismatch));
        let mut unknown = millimeters(10.);
        unknown.input("2yd+", f64::is_finite);
        assert_eq!(unknown.confirm(), Err(NumericError::UnsupportedUnit));
        assert_eq!(unknown.baseline(), 10.);
    }

    #[test]
    fn precision_numeric_invalid_and_zero_division_keep_preview_but_cannot_confirm() {
        let mut draft = millimeters(13100.);
        draft.input("2*", f64::is_finite);
        draft.input("0/", f64::is_finite);
        assert_eq!(draft.error(), Some(NumericError::DivisionByZero));
        assert_eq!(draft.preview(), 0.);
        assert_eq!(draft.baseline(), 26200.);
        assert!(!draft.is_confirmable());
        assert_eq!(draft.confirm(), Err(NumericError::DivisionByZero));
        draft.select_all();
        draft.input("bad", f64::is_finite);
        assert_eq!(draft.preview(), 0.);
        assert!(!draft.is_confirmable());
        draft.select_all();
        draft.input("4/", f64::is_finite);
        assert_eq!(draft.confirm(), Ok(6550.));
    }

    #[test]
    fn precision_numeric_selection_backspace_and_clear_retain_only_preview() {
        let mut draft = millimeters(4.);
        draft.input("12", f64::is_finite);
        draft.backspace(f64::is_finite);
        assert_eq!(draft.text(), "1");
        assert_eq!(draft.confirm(), Ok(1.));
        draft.select_all();
        draft.backspace(f64::is_finite);
        assert_eq!(draft.text(), "");
        assert_eq!(draft.confirm(), Err(NumericError::Incomplete));
        assert_eq!(draft.preview(), 1.);
        draft.input("-.5", f64::is_finite);
        assert_eq!(draft.confirm(), Ok(-0.5));
        draft.clear();
        assert_eq!(draft.preview(), -0.5);
        assert!(!draft.is_confirmable());
        draft.input("2*", f64::is_finite);
        assert_eq!(draft.confirm(), Ok(8.));
    }

    #[test]
    fn precision_numeric_nonfinite_and_missing_operands_never_become_confirmable() {
        assert_eq!(
            NumericDraft::new(f64::NAN, Quantity::Angle),
            Err(NumericError::NonFinite)
        );
        let mut draft = millimeters(10.);
        draft.input("NaN", f64::is_finite);
        assert_eq!(draft.error(), Some(NumericError::NonFinite));
        assert_eq!(draft.preview(), 10.);
        for text in ["-", "+", ".", "*", "/", "--2", "1e"] {
            draft.select_all();
            draft.input(text, f64::is_finite);
            assert!(!draft.is_confirmable(), "{text}");
        }
        draft.select_all();
        draft.input("2.3.4", f64::is_finite);
        assert_eq!(draft.error(), Some(NumericError::InvalidNumber));
        draft.select_all();
        draft.input("1e", f64::is_finite);
        assert_eq!(draft.error(), Some(NumericError::Incomplete));
        let mut overflow = millimeters(f64::MAX);
        overflow.input("2*", f64::is_finite);
        assert_eq!(overflow.error(), Some(NumericError::NonFinite));
        assert_eq!(overflow.preview(), 2.);
        assert_eq!(overflow.baseline(), f64::MAX);
    }

    #[test]
    fn precision_numeric_postfix_is_not_an_infix_expression() {
        let mut draft = millimeters(10.);
        draft.input("2+3", f64::is_finite);
        assert_eq!(draft.baseline(), 12.);
        assert_eq!(draft.confirm(), Ok(3.));
        draft.input("+", f64::is_finite);
        assert_eq!(draft.confirm(), Ok(15.));
    }
}
