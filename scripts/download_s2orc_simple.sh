#!/bin/bash

# Simple S2ORC downloader script
# Downloads S2ORC metadata files containing citation data

echo "=== S2ORC Metadata Downloader ==="
echo
echo "This script will download S2ORC metadata files."
echo "Each file is ~5GB compressed and contains ~2M papers with full citation graphs."
echo

# Create data directory
mkdir -p data/s2orc

# S2ORC files are hosted on AWS S3 with public access
BASE_URL="https://ai2-s2orc.s3.us-west-2.amazonaws.com/20200705v1/full/metadata"

# Download first file as a test
FILE="s2orc-metadata-000.jsonl.gz"
URL="$BASE_URL/$FILE"
OUTPUT="data/s2orc/$FILE"

if [ -f "$OUTPUT" ]; then
    echo "✓ $FILE already exists"
    echo "  Size: $(du -h $OUTPUT | cut -f1)"
else
    echo "Downloading $FILE..."
    echo "  URL: $URL"
    echo "  This will take a while (~5GB file)..."
    echo
    
    # Download with curl, showing progress
    curl -L -o "$OUTPUT" "$URL"
    
    if [ $? -eq 0 ]; then
        echo
        echo "✓ Download complete!"
        echo "  File: $OUTPUT"
        echo "  Size: $(du -h $OUTPUT | cut -f1)"
    else
        echo "✗ Download failed!"
        exit 1
    fi
fi

echo
echo "Next steps:"
echo "1. Parse the downloaded file: python3 scripts/parse_s2orc.py"
echo "2. Download more files if needed (s2orc-metadata-001.jsonl.gz, etc.)"
echo
echo "To download all 100 files (~500GB), you can use:"
echo "  for i in {000..099}; do"
echo "    curl -L -o data/s2orc/s2orc-metadata-\$i.jsonl.gz \\"
echo "      $BASE_URL/s2orc-metadata-\$i.jsonl.gz"
echo "  done"