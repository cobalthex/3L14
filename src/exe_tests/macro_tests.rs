
#[cfg(test)]
mod enum_tests
{
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

    #[test]
    fn variant_count()
    {
        assert_eq!(TestEnum::variant_count(), 3);
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod flags_tests
{
    use proc_macros_3l14::Flags;

    #[repr(u8)]
    #[derive(Flags, Copy, Clone)]
    pub enum BasicEnum
    {
        A = 1,
        B = 2,
        C = 4,
        D = 64,
    }

    #[repr(u8)]
    #[derive(Flags, Copy, Clone)]
    pub enum FlagEnum
    {
        A = 0b0001,
        B = 0b0010,
        C = 0b0100,
        D = 0b1000,
    }

    #[test]
    fn basic()
    {
        let z = BasicEnum::A | BasicEnum::B;
        assert_eq!(u8::from(z), 3);
    }

    #[test]
    fn flags()
    {
        let flags = FlagEnum::A | FlagEnum::B;
        assert!(flags.has_flag(FlagEnum::A));
        assert!(flags.has_flag(FlagEnum::B));
        assert!(!flags.has_flag(FlagEnum::C));

        assert!(FlagEnum::all_flags().has_flag(FlagEnum::A | FlagEnum::B | FlagEnum::C | FlagEnum::D));
        assert_eq!(FlagEnum::all_flags() as u8, 0b1111);
        assert_eq!(FlagEnum::bits_used(), 4);
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod type_hash_tests
{
    use proc_macros_3l14::LayoutHash;

    #[derive(LayoutHash)]
    struct Unit;
    #[derive(LayoutHash)]
    struct Empty { }
    #[derive(LayoutHash)]
    struct Single { a: u32 }
    #[derive(LayoutHash)]
    enum Enum { A, B, C }
    #[derive(LayoutHash)]
    union Union { a: u8, b: u16, c: f32 }

    #[test]
    fn test()
    {
        let _ = Unit::TYPE_LAYOUT_HASH;
        let _ = Empty::TYPE_LAYOUT_HASH;
        let _ = Single::TYPE_LAYOUT_HASH;
        let _ = Enum::TYPE_LAYOUT_HASH;
        let _ = Union::TYPE_LAYOUT_HASH;
    }
}
