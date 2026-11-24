#!/usr/bin/env python3
"""Inspect local GGUF metadata/tensor types without loading tensor weights."""
import argparse
from collections import Counter
import json
import struct


class Reader:
    def __init__(self, file):
        self.file = file

    def exact(self, n):
        data = self.file.read(n)
        if len(data) != n:
            raise ValueError('Truncated GGUF header')
        return data

    def number(self, fmt):
        return struct.unpack('<' + fmt, self.exact(struct.calcsize('<' + fmt)))[0]

    def string(self):
        n = self.number('Q')
        if n > 16 * 1024 * 1024:
            raise ValueError('GGUF string exceeds inspection limit')
        return self.exact(n).decode('utf8', errors='strict')

    def value(self, kind, keep=True, depth=0):
        formats = {0:'B', 1:'b', 2:'H', 3:'h', 4:'I', 5:'i', 6:'f', 7:'?', 10:'Q', 11:'q', 12:'d'}
        if kind == 8:
            return self.string()
        if kind == 9:
            if depth > 1:
                raise ValueError('Nested GGUF array exceeds inspection limit')
            element, count = self.number('I'), self.number('Q')
            if count > 2_000_000:
                raise ValueError('GGUF array exceeds inspection limit')
            values = []
            for _ in range(count):
                value = self.value(element, count <= 64, depth + 1)
                if keep and count <= 64:
                    values.append(value)
            return values if count <= 64 else {'array_length': count, 'element_type': element}
        if kind not in formats:
            raise ValueError('Unsupported GGUF metadata type')
        return self.number(formats[kind])


def inspect(path):
    with open(path, 'rb') as file:
        r = Reader(file)
        if r.exact(4) != b'GGUF' or r.number('I') != 3:
            raise ValueError('Expected little-endian GGUF v3')
        tensors, count = r.number('Q'), r.number('Q')
        if tensors > 1_000_000 or count > 100_000:
            raise ValueError('GGUF header count exceeds inspection limit')
        metadata = {}
        for _ in range(count):
            key = r.string()
            metadata[key] = r.value(r.number('I'))
        types = Counter()
        for _ in range(tensors):
            r.string()
            dims = r.number('I')
            if dims > 4:
                raise ValueError('Invalid GGUF tensor dimension count')
            for _ in range(dims):
                r.number('Q')
            types[r.number('I')] += 1
            r.number('Q')
        return {'version': 3, 'tensor_count': tensors, 'metadata': metadata,
                'tensor_type_counts': dict(types), 'header_bytes': file.tell(),
                'weights_loaded': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('file')
    args = parser.parse_args()
    print(json.dumps(inspect(args.file), indent=2))
