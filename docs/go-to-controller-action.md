# Go to controller action from a route

In a route file or any PHP file, put the cursor on the method string and run Go to Definition:

    Route::get('/p/{p}', [PostController::class, 'show']);   // cursor on 'show'
    Route::get('/p', 'Admin\PostController@index');          // cursor on the class or the method

It opens `public function show(` in `app/Http/Controllers/**/PostController.php`. The class and the
method string must be on the same line. Not resolved: inherited methods, invokable controllers, other
controller folders. Limits: depth 6, 2000 files, 1 MB per file, symlinks skipped. Nothing is executed.
If nothing is found, normal Go to Definition runs.
