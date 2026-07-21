import "reflect"

struct Any {
    value: *void,
    type_info: *TypeInfo
}

// fn any[T](val: *T) -> Any {
//     return Any.{
//         value: autocast val,
//         type_info: type_info(T)
//     };
// }
