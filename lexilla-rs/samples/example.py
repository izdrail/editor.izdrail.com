# A comment: café 🌍
def greet(name):
    """A multiline
    docstring."""
    value = f"hello {name}"
    if len(name) > 2:
        return value + str(42)
