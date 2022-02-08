pub mod ovec {
    #![macro_use]

    #[macro_export]
    macro_rules! ovec {
        ($($x:expr),*) => (vec![$($x.into()),*]);
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn it_works() {
            let x: Vec<String> = dbg!(ovec!["this", "that", "the other"]);
            assert_eq!(
                x,
                vec![
                    String::from("this"),
                    String::from("that"),
                    String::from("the other"),
                ]
            );
        }
    }
}
