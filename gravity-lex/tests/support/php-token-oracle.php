<?php
// Input is data. token_get_all without TOKEN_PARSE never executes it.
$source = stream_get_contents(STDIN);
$offset = 0;
$out = [];
foreach (token_get_all($source) as $token) {
    $name = is_array($token) ? token_name($token[0]) : 'CHAR';
    $text = is_array($token) ? $token[1] : $token;
    $out[] = [$name, $offset, strlen($text)];
    $offset += strlen($text);
}
echo json_encode(['php' => PHP_VERSION, 'bytes' => $offset, 'tokens' => $out], JSON_THROW_ON_ERROR);
