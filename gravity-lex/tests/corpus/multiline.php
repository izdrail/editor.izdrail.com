<section title="<?= $title ?>">λ</section>
<?php
/* a multiline
 comment containing ?> */
$x = <<<'TEXT'
raw $notInterpolated ?>
TEXT;
$y = <<<TEXT
value {$x}
TEXT;
echo "one
 two {$x}";
?>
<footer>tail</footer>
