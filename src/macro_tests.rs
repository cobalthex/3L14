use proc_macros_3l14::FancyEnum;

#[derive(FancyEnum)]
enum TestEnum
{
    #[enum_prop(foo = "5", bar = "donk")]
    A,
    B(i32),
    #[enum_prop(foo = "505")]
    C { a: f32, b: bool },
}

#[cfg(test)]
mod tests
{
    use super::TestEnum;

    #[test]
    fn variant_names()
    {
        assert_eq!("A", TestEnum::A.variant_name());
        assert_eq!("B", TestEnum::B(3).variant_name());
        assert_eq!("C", TestEnum::C{a:1.0,b:true}.variant_name());
    }

    #[test]
    fn variant_props()
    {
        assert_eq!(Some("5"), TestEnum::A.foo());
        assert_eq!(Some("donk"), TestEnum::A.bar());
        assert_eq!(None, TestEnum::B(3).foo());
        assert_eq!(Some("505"), TestEnum::C{a:1.0,b:true}.foo());
        assert_eq!(None, TestEnum::C{a:1.0,b:true}.bar());
    }
}