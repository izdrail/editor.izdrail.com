# Go to Eloquent factory

In a PHP file, put the cursor on `factory` in a static call on a model and run Go to Definition:

    Post::factory()->count(3)->create();   // opens class PostFactory in database/factories/PostFactory.php

Works for `\App\Models\Post::factory()` too. Only the conventional file name
`database/factories/{Model}Factory.php` is tried, so models that override `newFactory()` or use
`#[UseFactory]` are not followed, and `$post->factory()` is ignored. Limit: 1 MB per file. Nothing is
executed. If nothing is found, normal Go to Definition runs.
