
pub trait FlagsEnum<TSelf>
{
    type Repr;
    
    fn bits_used_trait() -> u8;
    fn get_flag_for_bit(self, bit: u8) -> Option<TSelf>;
}

pub struct FlagsEnumIter<TEnum: FlagsEnum<TEnum> + Copy>
{
    e: TEnum,
    next_bit: u8,
}
impl<TEnum: FlagsEnum<TEnum> + Copy> FlagsEnumIter<TEnum>
{
    #[inline] #[must_use] pub const fn new(e: TEnum) -> Self { Self { e, next_bit: 0 } }
}
impl<TEnum: FlagsEnum<TEnum> + Copy> Iterator for FlagsEnumIter<TEnum>
{
    type Item = TEnum;
    fn next(&mut self) -> Option<Self::Item>
    {
        let bits_used = TEnum::bits_used_trait();
        while self.next_bit < bits_used
        {
            let test_bit = self.next_bit;
            self.next_bit += 1;
            if let Some(f) = self.e.get_flag_for_bit(test_bit) { return Some(f); }
        }

        None
    }
}