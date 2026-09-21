macro_rules! enum_cases {
    (
        fn $name:ident($value:ident: $enum_type:ty) $verify:block
        cases {
            $(
                $(#[$attribute:meta])*
                $case:ident: $head:ident $(:: $tail:ident)+
                $(($($tuple:tt)*))? $({$($fields:tt)*})? => $fixture:expr
            ),+ $(,)?
        }
    ) => {
        mod $name {
            use super::*;

            #[deny(unreachable_patterns)]
            fn verify($value: $enum_type) {
                match &$value {
                    $($head $(:: $tail)+ $(($($tuple)*))? $({$($fields)*})? => (),)+
                }
                $verify
            }

            $(
                #[test]
                $(#[$attribute])*
                fn $case() {
                    let value: $enum_type = $fixture;
                    assert!(
                        matches!(&value, $head $(:: $tail)+ $(($($tuple)*))? $({$($fields)*})?),
                        "fixture does not match {}",
                        stringify!($head $(:: $tail)+ $(($($tuple)*))? $({$($fields)*})?),
                    );
                    verify(value);
                }
            )+
        }
    };
}

pub(crate) use enum_cases;
