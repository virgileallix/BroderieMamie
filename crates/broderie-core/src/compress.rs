//! Décompression utilisée par les formats Husqvarna (HUS) et Pfaff (VIP).
//! Portage fidèle de `EmbCompress` de pyembroidery (codage de Huffman + références arrière).

struct Huffman {
    default_value: usize,
    lengths: Vec<usize>,
    table: Vec<usize>,
    width: usize,
}

impl Huffman {
    fn constant(value: usize) -> Self {
        Huffman { default_value: value, lengths: Vec::new(), table: Vec::new(), width: 0 }
    }

    fn build(lengths: Vec<usize>, default_value: usize) -> Self {
        let width = lengths.iter().copied().max().unwrap_or(0).min(16);
        let mut table = Vec::new();
        let mut size = 1usize << width;
        for bit_length in 1..=width {
            size /= 2;
            for (i, &l) in lengths.iter().enumerate() {
                if l == bit_length {
                    table.extend(std::iter::repeat_n(i, size));
                }
            }
        }
        Huffman { default_value, lengths, table, width }
    }

    /// Renvoie (valeur, nombre de bits lus) pour les 16 prochains bits.
    fn lookup(&self, v16: u32) -> (usize, usize) {
        if self.lengths.is_empty() {
            return (self.default_value, 0);
        }
        let idx = (v16 >> (16 - self.width)) as usize;
        match self.table.get(idx) {
            Some(&v) => (v, self.lengths[v]),
            None => (self.default_value, 0),
        }
    }
}

struct Decoder<'a> {
    data: &'a [u8],
    pos: usize, // en bits
    block_elements: i64,
    characters: Huffman,
    distances: Huffman,
}

impl<'a> Decoder<'a> {
    fn bits(&self, start: usize, length: usize) -> u32 {
        if length == 0 {
            return 0;
        }
        let end = start + length - 1;
        let mut value: u64 = 0;
        for i in start / 8..=end / 8 {
            value = (value << 8) | *self.data.get(i).unwrap_or(&0) as u64;
        }
        let unused_right = (8 - (end + 1) % 8) % 8;
        ((value >> unused_right) & ((1u64 << length) - 1)) as u32
    }
    fn peek(&self, n: usize) -> u32 {
        self.bits(self.pos, n)
    }
    fn pop(&mut self, n: usize) -> u32 {
        let v = self.peek(n);
        self.pos += n;
        v
    }
    fn read_variable_length(&mut self) -> usize {
        let mut m = self.pop(3) as usize;
        if m != 7 {
            return m;
        }
        for _ in 0..13 {
            if self.pop(1) == 1 {
                m += 1;
            } else {
                break;
            }
        }
        m
    }
    fn load_character_length_huffman(&mut self) -> Huffman {
        let count = self.pop(5) as usize;
        if count == 0 {
            return Huffman::constant(self.pop(5) as usize);
        }
        let mut lengths = vec![0; count];
        let mut i = 0;
        while i < count {
            if i == 3 {
                i += self.pop(2) as usize;
                if i >= count {
                    break;
                }
            }
            lengths[i] = self.read_variable_length();
            i += 1;
        }
        Huffman::build(lengths, 8)
    }
    fn load_character_huffman(&mut self, length_huffman: &Huffman) -> Huffman {
        let count = self.pop(9) as usize;
        if count == 0 {
            return Huffman::constant(self.pop(9) as usize);
        }
        let mut lengths = vec![0; count];
        let mut i = 0;
        while i < count {
            let (c, l) = length_huffman.lookup(self.peek(16));
            self.pos += l;
            match c {
                0 => i += 1,
                1 => i += 3 + self.pop(4) as usize,
                2 => i += 20 + self.pop(9) as usize,
                _ => {
                    lengths[i] = c - 2;
                    i += 1;
                }
            }
        }
        Huffman::build(lengths, 0)
    }
    fn load_distance_huffman(&mut self) -> Huffman {
        let count = self.pop(5) as usize;
        if count == 0 {
            return Huffman::constant(self.pop(5) as usize);
        }
        let lengths = (0..count).map(|_| self.read_variable_length()).collect();
        Huffman::build(lengths, 0)
    }
    fn load_block(&mut self) {
        self.block_elements = self.pop(16) as i64;
        let length_huffman = self.load_character_length_huffman();
        self.characters = self.load_character_huffman(&length_huffman);
        self.distances = self.load_distance_huffman();
    }
    fn token(&mut self) -> usize {
        if self.block_elements <= 0 {
            self.load_block();
        }
        self.block_elements -= 1;
        let (v, l) = self.characters.lookup(self.peek(16));
        self.pos += l;
        v
    }
    fn position(&mut self) -> usize {
        let (v, l) = self.distances.lookup(self.peek(16));
        self.pos += l;
        if v == 0 {
            return 0;
        }
        let v = (v - 1).min(30);
        (1usize << v) + self.pop(v) as usize
    }
}

/// Décompresse `data` ; s'arrête après `size` octets au plus.
pub fn expand(data: &[u8], size: usize) -> Vec<u8> {
    let mut d = Decoder {
        data,
        pos: 0,
        block_elements: -1,
        characters: Huffman::constant(0),
        distances: Huffman::constant(0),
    };
    let mut out: Vec<u8> = Vec::with_capacity(size.min(1 << 24));
    let total_bits = data.len() * 8;
    let mut guard = 0usize;
    while total_bits > d.pos && out.len() <= size {
        guard += 1;
        if guard > size.saturating_mul(4) + 1024 {
            break; // données incohérentes : on s'arrête plutôt que de tourner en rond
        }
        let c = d.token();
        if c <= 255 {
            out.push(c as u8);
        } else if c == 510 {
            break;
        } else {
            let length = c - 253;
            let back = d.position() + 1;
            if back > out.len() {
                break;
            }
            let start = out.len() - back;
            for i in start..start + length {
                out.push(out[i]);
            }
        }
    }
    out
}
