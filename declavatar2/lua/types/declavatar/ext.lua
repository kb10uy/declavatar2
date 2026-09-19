---@meta declavatar.ext

--- Extension helpers for declaration scripts.
---
--- These are plain Lua rather than host features. The implementation ships inside the
--- declavatar2 binary, so this file carries the types alone.
local ext = {}

--- Returns the numbers from `from` to `to`, stepping by `step`.
---
--- The list is empty when the step never reaches the end, and a step of zero is an error.
---@param from number
---@param to number
---@param step? number Defaults to 1.
---@return number[]
function ext.range(from, to, step) end

return ext
