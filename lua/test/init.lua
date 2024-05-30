local T = {}
_G.T = T

-- Provided by `test.nvim`
--
-- Print and return value
--
---@generic T : any
---@param x T
---@return T
function T.debug(x)
  print(vim.inspect(x))
  return x
end

-- Provided by `test.nvim`
--
-- Like `assert`, but would print out the value of the expression if it is truthy
--
---@param expr any
---@param message? string
function T.assert(expr, message)
  if not expr then
    error(
      message
        or (
          "Assertion failed. Expected truthy value, but got "
          .. vim.inspect(expr)
        )
    )
  end
end

-- Provided by `test.nvim`
--
-- Throws error if the input function does not throw an error
--
---@param fn function
---@param message? string
function T.assert_error(fn, message)
  local ok, result = pcall(fn)
  if ok then
    error(message or "Expected an error, but got " .. vim.inspect(result))
  end
end

-- Provided by `test.nvim`
--
-- Like `assert`, but would throw error and print out the value of the expression is falsey
--
-- @param expr any
-- @param message? string
function T.assert_not(expr, message)
  if expr then
    error(
      message
        or (
          "Assertion failed. Expected falsey value, but got "
          .. vim.inspect(expr)
        )
    )
  end
end

-- Provided by `test.nvim`
--
-- Throws error and print out the input values if they are not equal
--
---@param lhs any
---@param rhs any
---@param message? string
function T.assert_eq(lhs, rhs, message)
  if lhs ~= rhs then
    local msg = message
      or (
        "Assertion failed. Expected "
        .. vim.inspect(rhs)
        .. ", but got "
        .. vim.inspect(lhs)
      )
    error(msg)
  end
end

-- Provided by `test.nvim`
--
-- Throws error and print out the input values if they are equal
--
---@param lhs any
---@param rhs any
---@param message? string
function T.assert_not_eq(lhs, rhs, message)
  if lhs == rhs then
    local msg = message
      or (
        "Assertion failed. Expected "
        .. vim.inspect(rhs)
        .. ", but got "
        .. vim.inspect(lhs)
      )
    error(msg)
  end
end

-- Provided by `test.nvim`
--
-- Like `assert_eq`, but uses `vim.deep_equal` to compare against the two input values
--
---@param lhs any
---@param rhs any
---@param message? string
function T.assert_deep_eq(lhs, rhs, message)
  if not vim.deep_equal(lhs, rhs) then
    local msg = message
      or (
        "Assertion failed. Expected "
        .. vim.inspect(rhs)
        .. ", but got "
        .. vim.inspect(lhs)
      )
    error(msg)
  end
end

-- Provided by `test.nvim`
--
-- Like `assert_not_eq`, but uses `vim.deep_equal` to compare against the two input values
--
---@param lhs any
---@param rhs any
---@param message? string
function T.assert_not_deep_eq(lhs, rhs, message)
  if vim.deep_equal(lhs, rhs) then
    local msg = message
      or (
        "Assertion failed. Expected "
        .. vim.inspect(rhs)
        .. ", but got "
        .. vim.inspect(lhs)
      )
    error(msg)
  end
end

-- Provided by `test.nvim`
--
-- Throws error if the input item is not contained by the input list
--
---@param list table
---@param item any
---@param message? string
function T.assert_contains(list, item, message)
  for _, v in ipairs(list) do
    if v == item then return end
  end

  local msg = message
    or (
      "Assertion failed. Expected "
      .. vim.inspect(item)
      .. " to be in the list "
      .. vim.inspect(list)
    )
  error(msg)
end

-- Provided by `test.nvim`
--
-- Throws error if the input item is contained by the input list
--
---@param list table
---@param item any
---@param message? string
function T.assert_not_contains(list, item, message)
  for _, v in ipairs(list) do
    if v == item then
      local msg = message
        or (
          "Assertion failed. Expected "
          .. vim.inspect(item)
          .. " to not be in the list "
          .. vim.inspect(list)
        )
      error(msg)
    end
  end
end
