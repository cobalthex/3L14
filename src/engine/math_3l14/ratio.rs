use std::ops::{Div, Mul};
use bitcode::{Decode, Encode};

// rename as Fraction?

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct Ratio<T>
{
    // allow negative?
    pub numerator: T,
    pub denominator: T,
}
impl<T> Ratio<T>
{
    #[inline] #[must_use]
    pub fn new(numerator: T, denominator: T) -> Self { Self { numerator, denominator } }
    // from_seconds

    #[inline] #[must_use]
    pub fn reciprocal(self) -> Self
    {
        Self { numerator: self.denominator, denominator: self.numerator }
    }

    #[inline] #[must_use]
    pub fn scale(&self, value: T) -> T
        where T: Copy + Mul<T, Output = T> + Div<T, Output = T>
    {
        (self.numerator * value) / self.denominator
    }

    // reciprocal scale?
    #[inline] #[must_use]
    pub fn inverse_scale(&self, value: T) -> T
        where T: Copy + Mul<T, Output = T> + Div<T, Output = T>
    {
        (self.denominator * value) / self.numerator
    }
}

#[cfg(test)]
mod tests
{
    use super::*;
    
    #[test]
    fn reciprocal()
    {
        let ratio = Ratio::new(1, 2);
        assert_eq!(ratio.reciprocal(), Ratio::new(2, 1));
    }

    #[test]
    fn scale()
    {
        let ratio = Ratio::new(1, 2);
        assert_eq!(ratio.scale(6), 3);
        assert_eq!(ratio.inverse_scale(6), 12);
    }
}