<?php
readonly class Profile {
    public private(set) string $name;
    public string $label {
        get => $this->name;
        set(string $value) { $this->name = $value; }
    }
}
