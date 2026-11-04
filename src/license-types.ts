// 授权状态(与 server/src/license.rs 的 LicenseStatus 对应)
export interface LicenseStatus {
    licensed: boolean;
    name: string;
    expire: string;
    trial_days_left: number;
    trial_expired: boolean;
}
