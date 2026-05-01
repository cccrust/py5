# Test pip installed packages
import certifi

print("=== Testing pip installed packages ===")
print("certifi imported successfully!")
print("certifi version:", certifi.__version__)

# Test core function
certs = certifi.where()
print("CA certificate path:", certs)

print("\n=== pip packages test passed! ===")