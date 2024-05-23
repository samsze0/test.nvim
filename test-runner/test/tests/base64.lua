local base64 = require("base64")

assert(base64.encode("Hello, World!") == "SGVsbG8sIFdvcmxkIQ==")
assert(base64.decode("SGVsbG8sIFdvcmxkIQ==") == "Hello, World!")