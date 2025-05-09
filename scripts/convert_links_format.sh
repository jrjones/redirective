#!/bin/bash
# Script to convert links.yaml from old format (pipe-delimited comments) to new format (YAML comments)
# (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License

set -euo pipefail

INPUT_FILE="${1:-links.yaml}"
OUTPUT_FILE="${2:-links.yaml.new}"

if [ ! -f "$INPUT_FILE" ]; then
    echo "Error: Input file $INPUT_FILE does not exist."
    echo "Usage: $0 [input_file] [output_file]"
    exit 1
fi

# Create a backup of the original file
cp "$INPUT_FILE" "$INPUT_FILE.bak"
echo "Created backup at $INPUT_FILE.bak"

# Make sure the output file is empty/new
> "$OUTPUT_FILE"

# Process the file line by line
while IFS= read -r line || [ -n "$line" ]; do  # The [ -n "$line" ] handles the last line if it doesn't end with a newline
    # Skip empty lines or already converted comments
    if [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]]; then
        echo "$line" >> "$OUTPUT_FILE"
        continue
    fi
    
    # Handle lines with pipe-delimited comments
    if [[ "$line" == *" | "* ]]; then
        # Split line at " | " and convert to YAML comment format
        key_value="${line%% | *}"
        comment="${line#* | }"
        echo "$key_value # $comment" >> "$OUTPUT_FILE"
    else
        # No comment delimiter, pass through as is
        echo "$line" >> "$OUTPUT_FILE"
    fi
done < "$INPUT_FILE"

echo "Conversion complete. New file created at $OUTPUT_FILE"
echo "To replace the original file, run: mv $OUTPUT_FILE $INPUT_FILE"