// TODO: isn't polymorphic in number of generic type parameters

#[macro_export]
macro_rules! id_cmp {
    ($name:ty, $field:ident, $gen:ident) => {
        impl<$gen> PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.$field == other.$field
            }
        }

        impl<$gen> Eq for $name {}

        impl<$gen> PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<$gen> Ord for $name {
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                self.$field.cmp(&other.$field)
            }
        }
    };
    ($name:ty, $field:ident) => {
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.$field == other.$field
            }
        }

        impl Eq for $name {}

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                self.$field.cmp(&other.$field)
            }
        }
    };
}
