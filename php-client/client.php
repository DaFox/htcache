<?php
// Sample client for the HTCache with HTTP interface
class Client {

    public function __construct(private string $endpoint) {}

    public function get(string $key): ?array {
        return json_decode(file_get_contents($this->endpoint . "/$key"), true);
    }

    public function set(string $key, int $ttl, array $data) {
        $context = stream_context_create([
            'http' => [
                'method' => 'PUT',
                'header' => "Content-Type: application/json\r\nX-TTL: $ttl",
                'content' => json_encode($data)
            ]
        ]);

        file_get_contents($this->endpoint . "/$key", false, $context);
    }
}


$client = new Client("http://localhost:3030");

$start = microtime(true);
if (!($data = $client->get("document-4712"))) {
    echo "Reading file from filesystem\n";
    $data = json_decode(file_get_contents("4712.json"), true);
    $client->set("document-4712", 120, $data);
}

$end = microtime(true);

var_dump($end - $start);
