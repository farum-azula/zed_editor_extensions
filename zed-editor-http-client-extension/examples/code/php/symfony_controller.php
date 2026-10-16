<?php
// Symfony.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
namespace App\Controller;

use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\Routing\Attribute\Route;

#[Route('/blog')]
class BlogController
{
    #[Route('/{slug}', methods: ['GET'])]
    public function show(Request $request)
    {
        $format = $request->query->get('format');
        return [];
    }

    #[Route('', methods: ['POST'])]
    public function create(Request $request)
    {
        $title = $request->request->get('title');
        $tenant = $request->headers->get('X-Tenant');
        return [];
    }
}
