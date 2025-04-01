use crate::{Error, inventory::Inventory};
use pso2packetlib::protocol::{
    ObjectHeader, Packet,
    items::{ChangeWeaponPalettePacket, EquipedWeaponPacket, Item},
    palette::{
        FullPaletteInfoPacket, LoadPalettePacket, NewDefaultPAsPacket, SetDefaultPAsPacket,
        SetPalettePacket, SetSubPalettePacket, SubPalette, UpdatePalettePacket,
        UpdateSubPalettePacket, WeaponPalette,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    cur_palette: u32,
    cur_subpalette: u32,
    cur_book: u32,
    palettes: [WeaponPalette; 6],
    subpalettes: [SubPalette; 6],
    default_pas: Vec<u32>,
}

impl Palette {
    pub fn send_palette(&self) -> Packet {
        Packet::LoadPalette(LoadPalettePacket {
            cur_palette: self.cur_palette,
            cur_subpalette: self.cur_subpalette,
            cur_book: self.cur_book,
            palettes: self.palettes.clone(),
            subpalettes: self.subpalettes.clone(),
        })
    }
    pub fn send_full_palette(&self) -> Packet {
        Packet::FullPaletteInfo(FullPaletteInfoPacket {
            cur_palette: self.cur_palette,
            cur_subpalette: self.cur_subpalette,
            cur_book: self.cur_book,
            palettes: self.palettes.clone(),
            subpalettes: self.subpalettes.clone(),
            default_pa: self.default_pas.clone().into(),
        })
    }
    pub fn send_default_pa(&self) -> Packet {
        Packet::NewDefaultPAs(NewDefaultPAsPacket {
            default: self.default_pas.clone().into(),
        })
    }
    pub fn set_palette(&mut self, packet: SetPalettePacket) -> Result<(), Error> {
        if packet.palette > 5 {
            return Err(Error::InvalidInput("set_palette"));
        }
        self.cur_palette = packet.palette;
        Ok(())
    }
    pub fn set_palette_data(&mut self, id: u32, palette: WeaponPalette) {
        self.palettes[id as usize] = palette;
    }
    pub fn set_subpalette_data(&mut self, palettes: [SubPalette; 6]) {
        self.subpalettes = palettes;
    }
    pub fn send_change_palette(&self, playerid: u32) -> Packet {
        Packet::ChangeWeaponPalette(ChangeWeaponPalettePacket {
            player: ObjectHeader {
                id: playerid,
                entity_type: pso2packetlib::protocol::ObjectType::Player,
                ..Default::default()
            },
            cur_palette: self.cur_palette,
            ..Default::default()
        })
    }
    pub fn send_cur_weapon(&self, playerid: u32, inv: &Inventory) -> Packet {
        let uuid = self.palettes[self.cur_palette as usize].uuid;
        Packet::EquipedWeapon(EquipedWeaponPacket {
            player: ObjectHeader {
                id: playerid,
                entity_type: pso2packetlib::protocol::ObjectType::Player,
                ..Default::default()
            },
            item: inv.get_inv_item(uuid).unwrap_or_default(),
        })
    }
    pub fn update_palette(
        &mut self,
        inv: &mut Inventory,
        packet: UpdatePalettePacket,
    ) -> Result<Packet, Error> {
        if packet.cur_palette > 5 {
            return Err(Error::InvalidInput("update_palette"));
        }
        let old = &self.palettes;
        let item = old[self.cur_palette as usize].uuid;
        if item != 0 {
            inv.unequip_item(item)?;
        }
        self.palettes = packet.palettes;
        let item = self.palettes[self.cur_palette as usize].uuid;
        if item != 0 {
            inv.equip_item(item, 9)?;
        }
        Ok(self.send_palette())
    }
    pub fn update_subpalette(&mut self, packet: UpdateSubPalettePacket) -> Result<Packet, Error> {
        if packet.cur_subpalette > 5 || packet.cur_book > 1 {
            return Err(Error::InvalidInput("update_subpalette"));
        }
        self.subpalettes = packet.subpalettes;
        Ok(self.send_palette())
    }
    pub fn set_subpalette(&mut self, packet: SetSubPalettePacket) -> Result<(), Error> {
        if packet.subpalette > 5 {
            return Err(Error::InvalidInput("set_subpalette"));
        }
        self.cur_subpalette = packet.subpalette;
        Ok(())
    }
    pub fn set_default_pas(&mut self, packet: SetDefaultPAsPacket) -> Packet {
        self.default_pas = packet.default.into();
        Packet::NewDefaultPAs(NewDefaultPAsPacket {
            default: self.default_pas.clone().into(),
        })
    }
    pub fn get_current_item(&self, inv: &Inventory) -> Result<Option<Item>, Error> {
        let uuid = self.palettes[self.cur_palette as usize].uuid;

        if uuid != 0 {
            Ok(Some(inv.get_inv_item(uuid)?))
        } else {
            Ok(None)
        }
    }
}
