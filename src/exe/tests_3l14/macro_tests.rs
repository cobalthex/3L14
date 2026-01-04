#![cfg(test)]
#![cfg(target_os = "windows")] // TODO: This is broken in (linux) CI for some reason

mod fancy_enum_tests
{
    use proc_macros_3l14::FancyEnum;

    // TODO: fix up enum_props, add optional override
    #[derive(FancyEnum)]
    enum TestEnum
    {
        #[enum_prop(quux = "a")]
        A,
        #[enum_prop(quux = "b")]
        B(i32),
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

mod sentence_case
{
    use proc_macros_3l14::pascal_to_title;

    struct FooBarBazBonk;

    #[test]
    fn test()
    {
        assert_eq!(pascal_to_title!(FooBarBazQuux), "Foo Bar Baz Quux");
        assert_eq!(pascal_to_title!("FooBarBazQuux"), "Foo Bar Baz Quux");
    }
}