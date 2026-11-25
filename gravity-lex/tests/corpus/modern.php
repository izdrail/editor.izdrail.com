<?php
#[Attribute]
readonly class Record {
    public function __construct(public string $name) {}
}
enum Status: string { case Ready = 'ready'; }
$value = match ($status) { Status::Ready => new Record('λ'), default => null };
$fn = fn(int|float $x): int|float => $x + 1;
