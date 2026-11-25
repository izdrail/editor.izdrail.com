<?php
namespace App;
use Vendor\Thing;
// ordinary comment
/** @param string $name */
function hello(?string $name): string { return 'hello '.$name; }
$x = 0xFF + 1_000 + .5e2;
$s = "value {$x}";
