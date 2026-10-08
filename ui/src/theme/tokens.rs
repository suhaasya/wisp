//! Raw design tokens (hex integers). The only place colour literals may appear.

/// Surface and text tokens shared by light/dark palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceTokens {
    pub canvas: u32,
    pub window: u32,
    pub sidebar: u32,
    pub panel: u32,
    pub raise: u32,
    pub ink1: u32,
    pub ink2: u32,
    pub ink3: u32,
    pub line: u32,
    pub line2: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub focus: u32,
    pub on_accent: u32,
    pub on_env: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvTokens {
    pub local: u32,
    pub dev: u32,
    pub staging: u32,
    pub production: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedEditTokens {
    pub modified: u32,
    pub modified_line: u32,
    pub inserted: u32,
    pub deleted: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxTokens {
    pub keyword: u32,
    pub string: u32,
    pub number: u32,
    pub function: u32,
    pub comment: u32,
    pub null: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTokens {
    pub surfaces: SurfaceTokens,
    pub env: EnvTokens,
    pub staged: StagedEditTokens,
    pub syntax: SyntaxTokens,
}

impl ColorTokens {
    pub const LIGHT: Self = Self {
        surfaces: SurfaceTokens {
            canvas: 0xE9ECF1,
            window: 0xFBFCFD,
            sidebar: 0xF1F3F7,
            panel: 0xFFFFFF,
            raise: 0xF6F7FA,
            ink1: 0x1D222B,
            ink2: 0x5A6372,
            ink3: 0x6E7788,
            line: 0xDDE2E9,
            line2: 0xE9ECF1,
            accent: 0x0E8A86,
            accent_soft: 0xD7F0EE,
            focus: 0x0E8A86,
            on_accent: 0x0B1F1E,
            on_env: 0xFFFFFF,
        },
        env: EnvTokens {
            local: 0x3A9D5D,
            dev: 0x356FBF,
            staging: 0xC98516,
            production: 0xCF4136,
        },
        staged: StagedEditTokens {
            modified: 0xFFF3C4,
            modified_line: 0xE2B93B,
            inserted: 0xDDF4E3,
            deleted: 0xFCE1DE,
        },
        syntax: SyntaxTokens {
            keyword: 0x7A3EC2,
            string: 0x2E7D46,
            number: 0xB4561B,
            function: 0x1F68C4,
            comment: 0x6E7788,
            null: 0xA2AAB8,
        },
    };

    pub const DARK: Self = Self {
        surfaces: SurfaceTokens {
            canvas: 0x0F1216,
            window: 0x171B21,
            sidebar: 0x14181D,
            panel: 0x1B2027,
            raise: 0x212731,
            ink1: 0xE4E8EE,
            ink2: 0xA2ABBA,
            ink3: 0x84909D,
            line: 0x2A313B,
            line2: 0x232932,
            accent: 0x3CC2BC,
            accent_soft: 0x16383A,
            focus: 0x3CC2BC,
            on_accent: 0x0B1F1E,
            on_env: 0xFFFFFF,
        },
        env: EnvTokens {
            local: 0x3A9D5D,
            dev: 0x356FBF,
            staging: 0xC98516,
            production: 0xCF4136,
        },
        staged: StagedEditTokens {
            modified: 0x3A3216,
            modified_line: 0xB8902A,
            inserted: 0x173322,
            deleted: 0x3D1D1B,
        },
        syntax: SyntaxTokens {
            keyword: 0xC192FF,
            string: 0x7BD08F,
            number: 0xF0A26B,
            function: 0x7DB3FF,
            comment: 0x84909D,
            null: 0x5F6878,
        },
    };
}
