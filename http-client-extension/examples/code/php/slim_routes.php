<?php
// Slim.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
use Slim\Factory\AppFactory;

$app = AppFactory::create();

$app->get('/books/{id}', function ($request, $response) {
    $expand = $request->getQueryParams()['expand'];
    return $response;
});

$app->post('/books', function ($request, $response) {
    $title = $request->getParsedBody()['title'];
    $tenant = $request->getHeaderLine('X-Tenant');
    return $response;
});

$app->delete('/books/{id}', function ($request, $response) {
    return $response;
});
