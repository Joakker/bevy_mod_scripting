let type = world.get_type_by_name.call("SimpleTupleStruct");
let constructed = construct.call(type, #{ "_0": 123 });

assert(constructed["_0"] == 123, "Value was constructed incorrectly, expected constructed.foo to be 123 but got " + constructed["_0"]);