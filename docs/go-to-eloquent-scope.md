# Go to Eloquent scope

In a PHP file, put the cursor on the method of a static call on a model and run Go to Definition:

    Post::published()->get();     // opens scopePublished() in app/Models/Post.php

Also matches the Laravel 12 `#[Scope]` attribute form (`#[Scope] protected function recent(...)`).
The model is read from `app/Models/Post.php`, then `app/Post.php`; a leading namespace is ignored.

Not resolved: chained calls (`$query->published()`), scopes from traits or parent classes, dynamic
`where*` methods. Real methods are left to normal Go to Definition. Limit: 1 MB per file. Nothing is
executed. If nothing is found, normal Go to Definition runs.
