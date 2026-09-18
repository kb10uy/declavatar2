local ext = {}

--- Returns the numbers from `from` to `to`, stepping by `step`, which defaults to 1.
function ext.range(from, to, step)
    step = step or 1
    if step == 0 then
        error("declavatar.ext.range: the step must not be zero", 2)
    end

    local values = {}
    local value = from
    while (step > 0 and value <= to) or (step < 0 and value >= to) do
        values[#values + 1] = value
        value = value + step
    end
    return values
end

return ext
