//! Tipos geométricos compartilhados.
//!
//! Todas as coordenadas são em **pixels físicos do espaço de tela virtual**: a
//! origem é o canto superior esquerdo do monitor primário, e monitores à
//! esquerda/acima produzem coordenadas negativas.

use serde::{Deserialize, Serialize};

/// Ponto no espaço de tela virtual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl From<(i32, i32)> for Point {
    fn from((x, y): (i32, i32)) -> Self {
        Self::new(x, y)
    }
}

/// Retângulo no espaço de tela virtual, com largura/altura sempre positivas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Constrói a partir de dois cantos quaisquer, normalizando a ordem.
    pub fn from_corners(a: Point, b: Point) -> Self {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        Self {
            x,
            y,
            width: a.x.abs_diff(b.x),
            height: a.y.abs_diff(b.y),
        }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// Interseção, ou `None` se os retângulos não se tocam.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x || bottom <= y {
            return None;
        }
        Some(Rect::new(
            x,
            y,
            (right - x) as u32,
            (bottom - y) as u32,
        ))
    }

    /// Menor retângulo que contém ambos.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Rect::new(x, y, (right - x) as u32, (bottom - y) as u32)
    }

    /// Converte um ponto do espaço virtual para coordenadas locais da imagem
    /// recortada por este retângulo. Retorna `None` se o ponto ficou de fora.
    pub fn to_local(&self, p: Point) -> Option<(u32, u32)> {
        if !self.contains(p) {
            return None;
        }
        Some(((p.x - self.x) as u32, (p.y - self.y) as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_corners_normaliza_ordem() {
        let r = Rect::from_corners(Point::new(100, 80), Point::new(20, 10));
        assert_eq!(r, Rect::new(20, 10, 80, 70));
    }

    #[test]
    fn to_local_respeita_origem_negativa() {
        // Monitor secundário à esquerda do primário.
        let r = Rect::new(-1920, 0, 1920, 1080);
        assert_eq!(r.to_local(Point::new(-1920, 0)), Some((0, 0)));
        assert_eq!(r.to_local(Point::new(-920, 500)), Some((1000, 500)));
        assert_eq!(r.to_local(Point::new(10, 10)), None);
    }

    #[test]
    fn intersect_sem_sobreposicao() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(10, 0, 10, 10);
        assert_eq!(a.intersect(&b), None);
        assert_eq!(a.union(&b), Rect::new(0, 0, 20, 10));
    }
}
