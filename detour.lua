-- OpenZT Lua Command Test Script
-- This script tests all migrated console commands
-- Run via: cd openzt-console && cargo run
-- Then execute: dofile("test_lua_commands.lua")

print("========================================")
print("OpenZT Lua Command Test Suite")
print("========================================\n")

local passed = 0
local failed = 0
local errors = {}

-- Helper function to test commands
local function test_command(name, func, expected_error)
    io.write("Testing " .. name .. "... ")
    local status, result, err = pcall(func)

    if not status then
        -- Lua error (pcall caught an exception)
        failed = failed + 1
        table.insert(errors, {name = name, error = tostring(result)})
        print("FAILED (Lua error: " .. tostring(result) .. ")")
    elseif err then
        -- Command returned (nil, error_string)
        if expected_error then
            passed = passed + 1
            print("PASSED (expected error: " .. err .. ")")
        else
            failed = failed + 1
            table.insert(errors, {name = name, error = err})
            print("FAILED (command error: " .. err .. ")")
        end
    else
        passed = passed + 1
        print("PASSED")
        if result then
            print("  Result: " .. tostring(result):sub(1, 80))
        end
    end
end

print("\n--- Core Functions ---")
test_command("help()", function() return help() end)
test_command("help('cash')", function() return help("cash") end)
test_command("continue()", function() return continue() end)

print("\n--- Game Management (ztgamemgr.rs) ---")
test_command("get_date()", function() return get_date() end)
test_command("add_cash(1000)", function() return add_cash(1000) end)
test_command("enable_dev_mode(true)", function() return enable_dev_mode(true) end)
test_command("enable_dev_mode(false)", function() return enable_dev_mode(false) end)
test_command("zoostats()", function() return zoostats() end)

print("\n--- Settings (settings.rs) ---")
test_command("list_settings()", function() return list_settings() end)
test_command("list_settings('AI')", function() return list_settings("AI") end)
-- Note: These may fail if settings don't exist
test_command("get_setting('AI', 'test')", function() return get_setting("AI", "test") end, true)
test_command("set_setting('AI', 'test', '100')", function() return set_setting("AI", "test", "100") end, true)

print("\n--- String Registry (string_registry.rs) ---")
test_command("get_string(9211)", function() return get_string(9211) end)
test_command("get_string(999999)", function() return get_string(999999) end, true)

print("\n--- Entity Types (bfentitytype.rs) ---")
test_command("sel_type()", function() return sel_type() end, true) -- May fail if nothing selected
test_command("make_sel(9500)", function() return make_sel(9500) end, true) -- May fail if not found

print("\n--- Expansions (expansions.rs) ---")
test_command("list_expansion()", function() return list_expansion() end)
test_command("get_current_expansion()", function() return get_current_expansion() end)
test_command("get_members()", function() return get_members() end)

print("\n--- Habitats (zthabitatmgr.rs) ---")
test_command("get_zthabitatmgr()", function() return get_zthabitatmgr() end)
test_command("list_exhibits()", function() return list_exhibits() end)

print("\n--- Terrain (ztadvterrainmgr.rs) ---")
test_command("list_bfterraintypeinfo()", function() return list_bfterraintypeinfo() end)

print("\n--- Registry (bfregistry.rs) ---")
test_command("list_bf_registry()", function() return list_bf_registry() end)

print("\n========================================")
print("Test Summary")
print("========================================")
print(string.format("Passed: %d", passed))
print(string.format("Failed: %d", failed))
print(string.format("Total:  %d", passed + failed))

if failed > 0 then
    print("\n--- Failed Tests ---")
    for i, err in ipairs(errors) do
        print(string.format("%d. %s", i, err.name))
        print("   Error: " .. err.error)
    end
end

print("\n========================================")
print("Test complete!")
print("========================================\n")

-- Return summary
return {
    passed = passed,
    failed = failed,
    total = passed + failed,
    errors = errors
}

