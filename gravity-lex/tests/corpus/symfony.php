<?php
use Symfony\Component\Routing\Attribute\Route;
#[Route('/items')]
class Items extends AbstractController {
    public function __invoke(Request $request): Response { return $this->render('items.html.twig'); }
}
#[ORM\Entity]
class Item { #[ORM\Id, ORM\Column] public int $id; #[Assert\NotBlank] public string $name; }
