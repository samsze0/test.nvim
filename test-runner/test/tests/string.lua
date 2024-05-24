local str_utils = require("utils.string")

local parts = str_utils.split("a,b,c", {
    sep = ","
})
assert(parts[1] == "a")
assert(parts[2] == "b")
assert(parts[3] == "c")