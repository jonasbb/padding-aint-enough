# README

Mounting a shared folder between KVM and host.

Setting SELinux permissions:

```bash
sudo semanage fcontext -a -t svirt_image_t (pwd)"/share(/.*)?"
sudo restorecon -vR "./share/"
```

Mounting in guest:

```bash
sudo mount -t 9p -o trans=virtio rpmbuild /mnt
```
