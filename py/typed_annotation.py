# Python Typed Annotations Test
# Test variable annotations, function parameter type hints, and return type annotations

# --- 1. Variable Annotations ---
x: int = 5
y: str = "hello"
z: float = 3.14
flag: bool = True

print("--- 1. Variable Annotations ---")
print(f"x = {x} (type: {type(x)})")
print(f"y = {y} (type: {type(y)})")
print(f"z = {z} (type: {type(z)})")
print(f"flag = {flag} (type: {type(flag)})")

# --- 2. Function with Parameter Type Hints ---
def greet(name: str, times: int) -> str:
    result = ""
    i = 0
    while i < times:
        result = result + f"Hello, {name}! "
        i = i + 1
    return result

print("\n--- 2. Function with Parameter Type Hints ---")
msg = greet("Alice", 2)
print(f"greet('Alice', 2) = {msg}")

# --- 3. Function with Return Type Annotation ---
def add(a: int, b: int) -> int:
    return a + b

def multiply(a: float, b: float) -> float:
    return a * b

print("\n--- 3. Function with Return Type Annotation ---")
print(f"add(3, 4) = {add(3, 4)}")
print(f"multiply(2.5, 4.0) = {multiply(2.5, 4.0)}")

# --- 4. Function with No Return Type ---
def log_message(msg: str, level: str) -> None:
    print(f"[{level}] {msg}")

print("\n--- 4. Function with No Return Type (None) ---")
result = log_message("System started", "INFO")
print(f"log_message returned: {result}")

# --- 5. Function with Complex Type Annotations ---
def process_list(items: list, multiplier: int) -> list:
    result = []
    for item in items:
        result.append(item * multiplier)
    return result

print("\n--- 5. Function with Complex Type Annotations ---")
numbers = [1, 2, 3]
processed = process_list(numbers, 10)
print(f"process_list([1,2,3], 10) = {processed}")

# --- 6. Default Values with Type Annotations ---
def create_user(name: str, age: int = 0, active: bool = True) -> dict:
    return {"name": name, "age": age, "active": active}

print("\n--- 6. Default Values with Type Annotations ---")
user1 = create_user("Bob")
user2 = create_user("Alice", 25)
user3 = create_user("Eve", 30, False)
print(f"create_user('Bob') = {user1}")
print(f"create_user('Alice', 25) = {user2}")
print(f"create_user('Eve', 30, False) = {user3}")

# --- 7. Function with Type Annotations and **kwargs ---
def config(info: dict, **kwargs: str) -> dict:
    result = info.copy()
    for key, value in kwargs.items():
        result[key] = value
    return result

print("\n--- 7. Function with Type Annotations and **kwargs ---")
base_config = {"debug": True}
full_config = config(base_config, port=8080, host="localhost")
print("config({'debug': True}, port=8080, host='localhost') =", full_config)

# --- 8. Lambda with annotations don't exist in Python, but we can use them ---
print("\n--- 8. Regular Operations (annotations don't affect execution) ---")
x: int = 100
y: int = 200
print(f"x: int = 100, y: int = 200")
print(f"x + y = {x + y}")

print("\n=== All Typed Annotation Tests Passed! ===")