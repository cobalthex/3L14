#[cfg(test)]
mod fancy_enum_tests
{
    use proc_macros_3l14::FancyEnum;

    // TODO: fix up enum_props, add optional override
    #[derive(FancyEnum)]
    enum TestEnum
    {
        // #[enum_prop(foo = "5", bar = "donk")]
        #[enum_prop(quux = "a")]
        A,
        #[enum_prop(quux = "b")]
        B(i32),
        // #[enum_prop(foo = "505")]
        #[enum_prop(quux = "c")]
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
        assert_eq!("a", TestEnum::A.quux());
        assert_eq!("b", TestEnum::B(3).quux());
        assert_eq!("c", TestEnum::C{a:1.0,b:true}.quux());
    }

    // todo: convert to optional
    // #[test]
    // fn variant_props()
    // {
    //     assert_eq!(Some("5"), TestEnum::A.foo());
    //     assert_eq!(Some("donk"), TestEnum::A.bar());
    //     assert_eq!(None, TestEnum::B(3).foo());
    //     assert_eq!(Some("505"), TestEnum::C{a:1.0,b:true}.foo());
    //     assert_eq!(None, TestEnum::C{a:1.0,b:true}.bar());
    // }

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
    #[derive(Flags)]
    pub enum BasicEnum
    {
        A = 1,
        B = 2,
        C = 4,
        D = 64,
    }

    #[repr(u8)]
    #[derive(Flags)]
    pub enum FlagEnum
    {
        A = 0b000001,
        B = 0b000010,
        C = 0b000100,
        D = 0b100000,
    }

    #[test]
    fn basic()
    {
        let z = BasicEnum::A | BasicEnum::B;
        assert_eq!(u8::from(z), 3);
    }

    #[test]
    fn none()
    {
        let n = BasicEnum::none();
        assert_eq!(u8::from(n), 0);
        assert_ne!(n, BasicEnum::A);
        assert_eq!(n | BasicEnum::A, BasicEnum::A);
    }

    #[test]
    fn flags()
    {
        let flags = FlagEnum::A | FlagEnum::B;
        assert!(flags.has_flag(FlagEnum::A));
        assert!(flags.has_flag(FlagEnum::B));
        assert!(!flags.has_flag(FlagEnum::C));

        assert!(FlagEnum::all_flags().has_flag(FlagEnum::A | FlagEnum::B | FlagEnum::C | FlagEnum::D));
        assert_eq!(FlagEnum::all_flags() as u8, 0b100111);
        assert_eq!(FlagEnum::bits_used(), 6);
    }

    #[test]
    fn iterating()
    {
        let flags = FlagEnum::B | FlagEnum::D;
        let mut iter = flags.iter_set_flags();
        assert_eq!(iter.next(), Some(FlagEnum::B));
        assert_eq!(iter.next(), Some(FlagEnum::D));
        assert_eq!(iter.next(), None);
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
