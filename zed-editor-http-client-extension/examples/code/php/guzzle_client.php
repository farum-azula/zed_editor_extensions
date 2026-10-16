<?php
// Guzzle.
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
use GuzzleHttp\Client;

$client = new Client(['base_uri' => getenv('API_URL')]);

function createPost(Client $client): void
{
    $client->request('POST', '/posts', [
        'json' => ['title' => 'demo', 'body' => 'text'],
        'headers' => ['X-Request-Id' => 'demo'],
    ]);
}

function listPosts(Client $client): void
{
    $client->get('/posts', ['query' => ['page' => 1]]);
}
