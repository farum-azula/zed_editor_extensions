<?php
// Laravel.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Route;

Route::get('/posts', function (Request $request) {
    return Post::paginate($request->query('per_page'));
});

Route::post('/posts', function (Request $request) {
    $data = $request->validate([
        'title' => 'required|string',
        'body' => 'required|string',
        'published' => 'boolean',
    ]);
    return Post::create($data);
});

Route::patch('/posts/{post}', [PostController::class, 'update']);

Route::delete('/posts/{post}', [PostController::class, 'destroy'])->middleware('auth:sanctum');
