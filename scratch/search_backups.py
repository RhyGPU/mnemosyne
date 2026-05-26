import os
import glob

search_dirs = [
    r"c:\Users\T-ROBOTICS\Downloads\mnemosyne",
    os.environ.get("TEMP", ""),
    os.environ.get("TMP", "")
]

for base_dir in search_dirs:
    if not base_dir:
        continue
    print(f"Searching in: {base_dir}")
    try:
        # Search recursively for files containing App.tsx
        for root, dirs, files in os.walk(base_dir):
            # Skip node_modules and .git
            if "node_modules" in root or ".git" in root:
                continue
            for file in files:
                if "App.tsx" in file or "App_bak" in file or "App.tsx~" in file:
                    full_path = os.path.join(root, file)
                    print(f"Found match: {full_path} ({os.path.getsize(full_path)} bytes)")
    except Exception as e:
        print(f"Error searching {base_dir}: {e}")
