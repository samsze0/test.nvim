-- Test math.abs
assert(math.abs(-5) == 5, "math.abs failed")

-- Test math.floor
assert(math.floor(5.7) == 5, "math.floor failed")

-- Test math.ceil
assert(math.ceil(5.3) == 6, "math.ceil failed")

-- Test math.sqrt
assert(math.sqrt(16) == 4, "math.sqrt failed")

-- Test math.pow
assert(math.pow(2, 3) == 8, "math.pow failed")

-- Test math.sin
assert(math.sin(math.pi / 2) == 1, "math.sin failed")

-- Test math.cos
assert(math.cos(math.pi) == -1, "math.cos failed")

-- Test math.tan
assert(math.tan(0) == 0, "math.tan failed")